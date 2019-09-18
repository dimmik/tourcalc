import React from 'react';
import { Link } from 'react-router-dom';

export default class Header extends React.Component {
    render() {
        return (
            <header>
                <menu>
                    <ul>
                        <li>
                            <Link to="/persons">Tour Persons</Link>
                        </li>
                        <li>
                            <Link to="/spendings">Tour Spendings</Link>
                        </li>
                        <li>
                            <Link to="/">Choose Tour</Link>
                        </li>
                        <li>
                            <Link to="/auth">Auth</Link>
                        </li>
                        <li> Chosen tour: {this.props.tour == null ? "n/a" : this.props.tour.name} </li>
                    </ul>
                </menu>
            </header>
        );
    }
};